// Small Windows installer. The music player itself is implemented in Rust/Slint.
using System;
using System.Drawing;
using System.IO;
using System.IO.Compression;
using System.Reflection;
using System.Windows.Forms;
using Microsoft.Win32;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        if (args.Length == 2 && args[0] == "--extract")
        {
            try { Extract(args[1]); }
            catch { Environment.ExitCode = 1; }
            return;
        }
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        Application.Run(new SetupForm());
    }

    internal static void Extract(string destination)
    {
        string root = Path.GetFullPath(destination).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
        Directory.CreateDirectory(root);
        using (Stream payload = Assembly.GetExecutingAssembly().GetManifestResourceStream("Payload"))
        using (ZipArchive archive = new ZipArchive(payload, ZipArchiveMode.Read))
        {
            foreach (ZipArchiveEntry entry in archive.Entries)
            {
                if (entry.FullName != "ZelenkaMusic.exe" && entry.FullName != "README.md" && entry.FullName != "Uninstall.ps1")
                    throw new InvalidDataException("Unexpected installer payload entry.");
                string file = Path.GetFullPath(Path.Combine(root, entry.FullName));
                if (!file.StartsWith(root, StringComparison.OrdinalIgnoreCase))
                    throw new InvalidDataException("Payload path escapes its destination.");
                using (Stream source = entry.Open())
                using (FileStream output = File.Create(file)) { source.CopyTo(output); }
            }
        }
        Directory.CreateDirectory(Path.Combine(root, "songs"));
    }
}

internal sealed class SetupForm : Form
{
    private readonly string destination = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Programs", "ZelenkaMusic");
    private readonly Button install = new Button();
    private readonly Button cancel = new Button();
    private readonly Label status = new Label();
    private readonly CheckBox launch = new CheckBox();
    private bool installed;

    internal SetupForm()
    {
        Text = "Установка ZelenkaMusic";
        ClientSize = new Size(560, 340);
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MaximizeBox = false;
        StartPosition = FormStartPosition.CenterScreen;
        BackColor = ColorTranslator.FromHtml("#101512");
        ForeColor = ColorTranslator.FromHtml("#F1F5F2");
        Font = new Font("Segoe UI", 10);
        Icon = Icon.ExtractAssociatedIcon(Assembly.GetExecutingAssembly().Location);
        Label brand = new Label { Text = "ZelenkaMusic", Left = 30, Top = 27, Width = 480, Height = 46, Font = new Font("Segoe UI", 25, FontStyle.Bold), ForeColor = ColorTranslator.FromHtml("#7BCE9A") };
        Label subtitle = new Label { Text = "Музыкальный проигрыватель", Left = 33, Top = 81, Width = 480, Height = 28, ForeColor = ColorTranslator.FromHtml("#A4B0A8") };
        Label location = new Label { Text = "Папка установки:\r\n" + destination, Left = 33, Top = 127, Width = 492, Height = 60, Font = new Font("Segoe UI", 9) };
        launch.Text = "Запустить ZelenkaMusic после установки";
        launch.Checked = true; launch.SetBounds(33, 202, 490, 29);
        status.SetBounds(33, 237, 492, 26);
        status.Font = new Font("Segoe UI", 9);
        status.ForeColor = ColorTranslator.FromHtml("#A4B0A8");
        status.Text = "Поместите альбомы в папку songs рядом с приложением.";
        cancel.Text = "Отмена"; cancel.SetBounds(272, 284, 118, 38);
        cancel.FlatStyle = FlatStyle.Flat; cancel.FlatAppearance.BorderSize = 0;
        cancel.BackColor = ColorTranslator.FromHtml("#222B25");
        cancel.Click += delegate { Close(); };
        install.Text = "Установить"; install.SetBounds(404, 284, 124, 38);
        install.FlatStyle = FlatStyle.Flat; install.FlatAppearance.BorderSize = 0;
        install.BackColor = ColorTranslator.FromHtml("#7BCE9A");
        install.ForeColor = ColorTranslator.FromHtml("#101512");
        install.Font = new Font("Segoe UI", 10, FontStyle.Bold);
        install.Click += InstallClicked;
        Controls.AddRange(new Control[] { brand, subtitle, location, launch, status, cancel, install });
        AcceptButton = install; CancelButton = cancel;
    }

    private void InstallClicked(object sender, EventArgs args)
    {
        if (installed)
        {
            if (launch.Checked) System.Diagnostics.Process.Start(Path.Combine(destination, "ZelenkaMusic.exe"));
            Close(); return;
        }
        install.Enabled = false; status.Text = "Установка…"; status.Refresh();
        try
        {
            Program.Extract(destination);
            string menu = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.Programs), "ZelenkaMusic");
            Directory.CreateDirectory(menu);
            string oldMusicShortcut = Path.Combine(menu, "Music folder.lnk");
            if (File.Exists(oldMusicShortcut)) File.Delete(oldMusicShortcut);
            Type shellType = Type.GetTypeFromProgID("WScript.Shell");
            dynamic shell = Activator.CreateInstance(shellType);
            dynamic appShortcut = shell.CreateShortcut(Path.Combine(menu, "ZelenkaMusic.lnk"));
            appShortcut.TargetPath = Path.Combine(destination, "ZelenkaMusic.exe");
            appShortcut.WorkingDirectory = destination;
            appShortcut.IconLocation = Path.Combine(destination, "ZelenkaMusic.exe") + ",0";
            appShortcut.Save();
            dynamic musicShortcut = shell.CreateShortcut(Path.Combine(menu, "Папка музыки.lnk"));
            musicShortcut.TargetPath = Path.Combine(destination, "songs");
            musicShortcut.Save();
            using (RegistryKey key = Registry.CurrentUser.CreateSubKey(@"Software\Microsoft\Windows\CurrentVersion\Uninstall\ZelenkaMusic"))
            {
                key.SetValue("DisplayName", "ZelenkaMusic");
                key.SetValue("DisplayVersion", "0.1.2");
                key.SetValue("Publisher", "ZelenkaMusic");
                key.SetValue("InstallLocation", destination);
                key.SetValue("DisplayIcon", Path.Combine(destination, "ZelenkaMusic.exe") + ",0");
                key.SetValue("UninstallString", "powershell.exe -NoProfile -ExecutionPolicy Bypass -File \"" + Path.Combine(destination, "Uninstall.ps1") + "\"");
                key.SetValue("NoModify", 1, RegistryValueKind.DWord);
                key.SetValue("NoRepair", 1, RegistryValueKind.DWord);
            }
            installed = true; install.Text = "Готово"; cancel.Visible = false;
            status.Text = "Установлено. Ярлыки добавлены в меню «Пуск».";
        }
        catch (Exception error)
        {
            status.Text = "Установка не завершена.";
            MessageBox.Show(this, "Закройте ZelenkaMusic, если он запущен, и попробуйте ещё раз.\r\n\r\n" + error.Message, "ZelenkaMusic", MessageBoxButtons.OK, MessageBoxIcon.Error);
        }
        finally { install.Enabled = true; }
    }
}
