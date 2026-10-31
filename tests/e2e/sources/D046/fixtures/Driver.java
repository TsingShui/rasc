package fixtures;

/**
 * D046 behavior Driver. Runs exactly one invocation id and prints one JSON
 * object with the invocation id and the observations read from the actual
 * static fields. Nothing is copied from the specification: the values are read
 * after the class is initialized.
 */
public final class Driver {
    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            System.err.println("usage: fixtures.Driver <invocation-id>");
            System.exit(2);
        }
        String invocation = args[0];
        if (!"v1".equals(invocation)) {
            System.err.println("unknown invocation id: " + invocation);
            System.exit(2);
        }
        String s = Case.s;
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"D046\",\"invocation\":\"").append(invocation).append('"');
        out.append(",\"result\":{\"kind\":\"void\"}");
        out.append(",\"heap\":{\"s\":");
        if (s == null) {
            out.append("null");
        } else {
            out.append("\"<non-null>\"");
        }
        out.append(",\"b\":").append(Case.b);
        out.append(",\"i\":").append(Case.i);
        out.append("},\"exception\":null,\"completed\":true}");
        System.out.println(out);
    }
}
