using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Net;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using System.Web.Script.Serialization;

// Shared by the native installer and the Inno Setup download window.
internal static class RepositoryContent
{
    internal const string Repository = "shakinessaim/zelenkamusic";

    internal static string Download(string destination, IProgress<string> progress, CancellationToken cancel,
        Func<string, CancellationToken, Dictionary<string, object>> getJson = null,
        Action<string, string, CancellationToken> fetchFile = null)
    {
        getJson = getJson ?? Json;
        fetchFile = fetchFile ?? Fetch;
        ServicePointManager.SecurityProtocol = SecurityProtocolType.Tls12;
        progress.Report("Проверка репозитория GitHub…");
        string api = "https://api.github.com/repos/" + Repository;
        var repo = getJson(api, cancel);
        var branch = getJson(api + "/branches/" + Uri.EscapeDataString((string)repo["default_branch"]), cancel);
        string commit = (string)((Dictionary<string, object>)branch["commit"])["sha"];
        var tree = getJson(api + "/git/trees/" + commit + "?recursive=1", cancel);
        if ((bool)tree["truncated"]) throw new IOException("Список файлов GitHub неполный. Загрузка не начата.");
        var files = new List<Dictionary<string, object>>();
        foreach (object value in (IEnumerable)tree["tree"])
        {
            var item = (Dictionary<string, object>)value;
            string path = (string)item["path"];
            if ((string)item["type"] == "blob" && (string)item["mode"] != "120000" &&
                (path.StartsWith("songs/", StringComparison.Ordinal) || path.StartsWith("lyrics/", StringComparison.Ordinal)))
                files.Add(item);
        }
        if (files.Count == 0) throw new IOException("В репозитории пока нет файлов songs или lyrics.");
        int downloaded = 0, existing = 0;
        foreach (var file in files)
        {
            cancel.ThrowIfCancellationRequested();
            string relative = (string)file["path"];
            string target = SafePath(destination, relative);
            progress.Report(String.Format("Файл {0}/{1}: {2}", downloaded + existing + 1, files.Count, relative));
            if (File.Exists(target)) { existing++; continue; }
            Directory.CreateDirectory(Path.GetDirectoryName(target));
            SafePath(destination, relative); // Also reject reparse points in newly created directories.
            string temporary = target + ".download-" + Guid.NewGuid().ToString("N");
            try
            {
                string encoded = String.Join("/", Array.ConvertAll(relative.Split('/'), Uri.EscapeDataString));
                fetchFile("https://raw.githubusercontent.com/" + Repository + "/" + commit + "/" + encoded, temporary, cancel);
                if (new FileInfo(temporary).Length != Convert.ToInt64(file["size"]))
                    throw new IOException("GitHub вернул неполный файл: " + relative);
                if (new FileInfo(temporary).Length < 1024)
                {
                    string pointer = File.ReadAllText(temporary);
                    if (pointer.StartsWith("version https://git-lfs.github.com/spec/v1\n", StringComparison.Ordinal))
                    {
                        long size = -1; string expectedHash = null;
                        foreach (string line in pointer.Split('\n'))
                        {
                            if (line.StartsWith("size ")) Int64.TryParse(line.Substring(5).Trim(), out size);
                            if (line.StartsWith("oid sha256:")) expectedHash = line.Substring(11).Trim();
                        }
                        if (size < 0 || expectedHash == null || expectedHash.Length != 64) throw new IOException("Некорректный Git LFS файл.");
                        fetchFile("https://media.githubusercontent.com/media/" + Repository + "/" + commit + "/" + encoded, temporary, cancel);
                        using (var hash = SHA256.Create())
                        using (var input = File.OpenRead(temporary))
                            if (new FileInfo(temporary).Length != size || BitConverter.ToString(hash.ComputeHash(input)).Replace("-", "").ToLowerInvariant() != expectedHash)
                                throw new IOException("Git LFS вернул неполный файл: " + relative);
                    }
                }
                cancel.ThrowIfCancellationRequested();
                SafePath(destination, relative);
                if (File.Exists(target)) { existing++; continue; }
                File.Move(temporary, target);
                downloaded++;
            }
            finally { if (File.Exists(temporary)) File.Delete(temporary); }
        }
        return String.Format("Загружено файлов: {0}. Уже существовали: {1}.", downloaded, existing);
    }

    internal static string SafePath(string destination, string relative)
    {
        if (!(relative.StartsWith("songs/", StringComparison.Ordinal) || relative.StartsWith("lyrics/", StringComparison.Ordinal)))
            throw new InvalidDataException("Неожиданная папка репозитория.");
        foreach (string component in relative.Split('/'))
        {
            string name = component.Split('.')[0].ToUpperInvariant();
            if (component.Length == 0 || component == "." || component == ".." || component.TrimEnd(' ', '.') != component ||
                component.IndexOfAny(Path.GetInvalidFileNameChars()) >= 0 || name == "CON" || name == "PRN" || name == "AUX" || name == "NUL" ||
                (name.Length == 4 && (name.StartsWith("COM") || name.StartsWith("LPT")) && name[3] >= '1' && name[3] <= '9'))
                throw new InvalidDataException("Имя файла несовместимо с Windows: " + relative);
        }
        string root = Path.GetFullPath(destination).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
        string path = Path.GetFullPath(Path.Combine(root, relative.Replace('/', Path.DirectorySeparatorChar)));
        if (!path.StartsWith(root, StringComparison.OrdinalIgnoreCase)) throw new InvalidDataException("Путь вне папки приложения.");
        string current = path;
        while (current != null && current.StartsWith(root.TrimEnd(Path.DirectorySeparatorChar), StringComparison.OrdinalIgnoreCase))
        {
            if ((File.Exists(current) || Directory.Exists(current)) && (File.GetAttributes(current) & FileAttributes.ReparsePoint) != 0)
                throw new InvalidDataException("Загрузка через ссылку на другую папку не поддерживается.");
            if (current.Equals(root.TrimEnd(Path.DirectorySeparatorChar), StringComparison.OrdinalIgnoreCase)) break;
            current = Path.GetDirectoryName(current);
        }
        return path;
    }

    private static Dictionary<string, object> Json(string url, CancellationToken cancel)
    {
        using (var response = Open(url, cancel))
        using (var input = response.GetResponseStream())
        using (var output = new MemoryStream())
        {
            byte[] buffer = new byte[65536]; int read;
            while ((read = input.Read(buffer, 0, buffer.Length)) != 0)
            {
                cancel.ThrowIfCancellationRequested();
                output.Write(buffer, 0, read);
                if (output.Length > 32 * 1024 * 1024) throw new IOException("Список файлов репозитория слишком большой.");
            }
            return new JavaScriptSerializer { MaxJsonLength = 32 * 1024 * 1024 }.Deserialize<Dictionary<string, object>>(Encoding.UTF8.GetString(output.ToArray()));
        }
    }

    private static HttpWebResponse Open(string url, CancellationToken cancel)
    {
        cancel.ThrowIfCancellationRequested();
        var request = (HttpWebRequest)WebRequest.Create(url);
        request.UserAgent = "ZelenkaMusic/0.1.3";
        request.Timeout = 30000; request.ReadWriteTimeout = 30000;
        try { using (cancel.Register(request.Abort)) { return (HttpWebResponse)request.GetResponse(); } }
        catch (WebException error)
        {
            cancel.ThrowIfCancellationRequested();
            var response = error.Response as HttpWebResponse;
            string explanation = response != null && response.StatusCode == HttpStatusCode.NotFound
                ? "Репозиторий или файл недоступен. Для загрузки репозиторий должен быть публичным."
                : response != null && (response.StatusCode == HttpStatusCode.Forbidden || (int)response.StatusCode == 429)
                    ? "GitHub ограничил загрузку. Попробуйте позже." : "Проверьте подключение к интернету.";
            if (response != null) response.Dispose();
            throw new IOException(explanation, error);
        }
    }

    private static void Fetch(string url, string target, CancellationToken cancel)
    {
        using (var response = Open(url, cancel))
        using (var input = response.GetResponseStream())
        using (var output = File.Create(target))
        {
            byte[] buffer = new byte[65536]; int read;
            while ((read = input.Read(buffer, 0, buffer.Length)) != 0)
            { cancel.ThrowIfCancellationRequested(); output.Write(buffer, 0, read); }
        }
    }
}
