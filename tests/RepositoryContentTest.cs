using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using System.Security.Cryptography;
using System.Threading;

internal static class RepositoryContentTest
{
    private sealed class Progress : IProgress<string> { public void Report(string message) { } }
    private static readonly List<Dictionary<string, object>> Files = new List<Dictionary<string, object>>();
    private static Dictionary<string, object> Json(string url, CancellationToken token)
    {
        if (url.EndsWith("/zelenkamusic")) return new Dictionary<string, object> { { "default_branch", "custom-branch" } };
        if (url.Contains("/branches/")) return new Dictionary<string, object> { { "commit", new Dictionary<string, object> { { "sha", "pinned-commit" } } } };
        return new Dictionary<string, object> { { "truncated", false }, { "tree", Files.ToArray() } };
    }
    private static void Add(string path, string text)
    {
        Files.Add(new Dictionary<string, object> { { "path", path }, { "type", "blob" }, { "mode", "100644" }, { "size", Encoding.UTF8.GetByteCount(text) } });
    }
    private static void Assert(bool condition, string message) { if (!condition) throw new Exception(message); }
    public static void Main(string[] args)
    {
        string root = Path.GetFullPath(args[0]); Directory.CreateDirectory(Path.Combine(root, "songs", "Album"));
        string existing = Path.Combine(root, "songs", "Album", "Keep.mp3"); File.WriteAllText(existing, "manual");
        Add("songs/Album/Keep.mp3", "remote"); Add("songs/Album/New.mp3", "new"); Add("lyrics/Album/New.txt", "lyrics"); Add("src/private.txt", "do not download");
        int requests = 0;
        RepositoryContent.Download(root, new Progress(), CancellationToken.None, Json, (url, path, token) => {
            Assert(url.Contains("/pinned-commit/"), "Files were not pinned to the manifest commit"); requests++;
            File.WriteAllBytes(path, Encoding.UTF8.GetBytes(url.EndsWith(".mp3") ? "new" : "lyrics"));
        });
        Assert(requests == 2, "Existing or unrelated files were downloaded");
        Assert(File.ReadAllText(existing) == "manual", "Existing music was overwritten");
        Assert(File.ReadAllText(Path.Combine(root, "lyrics", "Album", "New.txt")) == "lyrics", "Folder structure lost");
        foreach (string path in new[] { "songs/../escape", "lyrics/CON.txt", "songs/Album/bad.", "songs/Album/a:b", "src/file" }) {
            try { RepositoryContent.SafePath(root, path); throw new Exception("Unsafe path accepted: " + path); }
            catch (InvalidDataException) { }
        }
        Files.Clear(); Add("songs/Interrupted.mp3", "complete");
        try {
            RepositoryContent.Download(root, new Progress(), CancellationToken.None, Json, (url, path, token) => File.WriteAllText(path, "partial"));
            throw new Exception("Partial file accepted");
        } catch (IOException) { }
        Assert(!File.Exists(Path.Combine(root, "songs", "Interrupted.mp3")), "Partial download was installed");
        Assert(Directory.GetFiles(Path.Combine(root, "songs"), "*.download-*").Length == 0, "Temporary file leaked");
        var cancellation = new CancellationTokenSource();
        try {
            RepositoryContent.Download(root, new Progress(), cancellation.Token, Json, (url, path, token) => { File.WriteAllText(path, "complete"); cancellation.Cancel(); });
            throw new Exception("Cancellation ignored");
        } catch (OperationCanceledException) { }
        Assert(!File.Exists(Path.Combine(root, "songs", "Interrupted.mp3")), "Cancelled download was installed");
        Assert(Directory.GetFiles(Path.Combine(root, "songs"), "*.download-*").Length == 0, "Cancelled temporary file leaked");
        string data = "LFS audio bytes", expectedHash;
        using (var hash = SHA256.Create()) expectedHash = BitConverter.ToString(hash.ComputeHash(Encoding.UTF8.GetBytes(data))).Replace("-", "").ToLowerInvariant();
        string pointer = "version https://git-lfs.github.com/spec/v1\noid sha256:" + expectedHash + "\nsize " + Encoding.UTF8.GetByteCount(data) + "\n";
        Files.Clear(); Add("songs/Lfs.flac", pointer);
        RepositoryContent.Download(root, new Progress(), CancellationToken.None, Json,
            (url, path, token) => File.WriteAllText(path, url.Contains("media.githubusercontent.com") ? data : pointer));
        Assert(File.ReadAllText(Path.Combine(root, "songs", "Lfs.flac")) == data, "LFS pointer installed instead of audio");
        Files.Clear(); Add("songs/BadLfs.flac", pointer);
        try {
            RepositoryContent.Download(root, new Progress(), CancellationToken.None, Json,
                (url, path, token) => File.WriteAllText(path, url.Contains("media.githubusercontent.com") ? new string('x', data.Length) : pointer));
            throw new Exception("Bad LFS hash accepted");
        } catch (IOException) { }
        Assert(!File.Exists(Path.Combine(root, "songs", "BadLfs.flac")), "Corrupt LFS data was installed");
        Console.WriteLine("Repository downloads: pinned commit, preservation, folder selection, path validation, partial cleanup and cancellation passed.");
        Console.WriteLine("Git LFS media download and checksum rejection passed.");
    }
}
