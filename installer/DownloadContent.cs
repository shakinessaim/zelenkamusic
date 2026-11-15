using System;
using System.Drawing;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Forms;

internal static class DownloadProgram
{
    [STAThread]
    private static void Main(string[] args)
    {
        if (args.Length != 1) { Environment.ExitCode = 1; return; }
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        Application.Run(new DownloadForm(args[0]));
    }
}

internal sealed class DownloadForm : Form
{
    private readonly Label status = new Label();
    private readonly CancellationTokenSource cancellation = new CancellationTokenSource();
    private bool complete;
    internal DownloadForm(string destination)
    {
        Text = "ZelenkaMusic — загрузка музыки и текстов";
        ClientSize = new Size(540, 200); StartPosition = FormStartPosition.CenterScreen;
        FormBorderStyle = FormBorderStyle.FixedDialog; MaximizeBox = false;
        BackColor = ColorTranslator.FromHtml("#101512"); ForeColor = ColorTranslator.FromHtml("#F1F5F2");
        Font = new Font("Segoe UI", 10);
        status.SetBounds(24, 24, 492, 88); status.Text = "Подготовка загрузки…";
        Button cancel = new Button { Text = "Отмена", Left = 370, Top = 135, Width = 145, Height = 38 };
        cancel.Click += delegate { if (complete) Close(); else { cancellation.Cancel(); cancel.Enabled = false; status.Text = "Остановка…"; } };
        Controls.AddRange(new Control[] { status, cancel });
        FormClosing += delegate(object sender, FormClosingEventArgs e) { if (!complete) { cancellation.Cancel(); e.Cancel = true; } };
        Shown += async delegate {
            try {
                var progress = new Progress<string>(value => { if (!cancellation.IsCancellationRequested) status.Text = value; });
                status.Text = await Task.Run(() => RepositoryContent.Download(destination, progress, cancellation.Token));
            }
            catch (OperationCanceledException) { status.Text = "Загрузка отменена. Готовые файлы сохранены."; Environment.ExitCode = 1; }
            catch (Exception error) { status.Text = "Загрузка не завершена. " + error.Message; Environment.ExitCode = 1; }
            finally { complete = true; cancel.Text = "Готово"; cancel.Enabled = true; }
        };
    }
}
