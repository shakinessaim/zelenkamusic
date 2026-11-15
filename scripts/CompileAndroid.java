import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Locale;
import javax.tools.Diagnostic;
import javax.tools.DiagnosticCollector;
import javax.tools.JavaCompiler;
import javax.tools.JavaFileObject;
import javax.tools.StandardJavaFileManager;
import javax.tools.ToolProvider;

/** Isolated compiler process with explicit diagnostic and exit-status handling. */
public final class CompileAndroid {
    public static void main(String[] args) {
        if (args.length < 3) throw new IllegalArgumentException("Expected classpath, output directory, and source files");
        JavaCompiler compiler = ToolProvider.getSystemJavaCompiler();
        if (compiler == null) throw new IllegalStateException("A Java JDK is required");
        DiagnosticCollector<JavaFileObject> diagnostics = new DiagnosticCollector<>();
        StandardJavaFileManager files = compiler.getStandardFileManager(diagnostics, Locale.ROOT, StandardCharsets.UTF_8);
        Iterable<? extends JavaFileObject> sources = files.getJavaFileObjectsFromStrings(
                Arrays.asList(Arrays.copyOfRange(args, 2, args.length)));
        boolean success = compiler.getTask(null, files, diagnostics, Arrays.asList(
                "--release", "8", "-encoding", "UTF-8", "-Xlint:-options",
                "-classpath", args[0], "-d", args[1]), null, sources).call();
        for (Diagnostic<? extends JavaFileObject> diagnostic : diagnostics.getDiagnostics()) {
            if (diagnostic.getKind() == Diagnostic.Kind.ERROR || diagnostic.getKind() == Diagnostic.Kind.WARNING
                    || diagnostic.getKind() == Diagnostic.Kind.MANDATORY_WARNING)
                System.err.println(diagnostic);
        }
        // The JDK's ZIP filesystem close traverses ancestor paths with toRealPath().
        // Restricted Windows environments can reject that metadata operation even
        // after a successful compilation. This dedicated process exits here, so
        // the OS releases its file handles without file-manager shutdown traversal.
        if (success) System.out.println("Android Java compilation passed.");
        System.exit(success ? 0 : 1);
    }
}
