using System;
using System.Threading;
using System.Windows.Forms;

namespace AppSnapshot
{
    internal static class Program
    {
        [STAThread]
        private static void Main()
        {
            bool created;
            using (var singleInstance = new Mutex(true, "Local\\AppSnapshot.SingleInstance", out created))
            {
                if (!created)
                {
                    return;
                }

                NativeMethods.SetProcessDPIAware();
                Application.EnableVisualStyles();
                Application.SetCompatibleTextRenderingDefault(false);
                Application.Run(new SnapshotBubbleForm());
            }
        }
    }
}
