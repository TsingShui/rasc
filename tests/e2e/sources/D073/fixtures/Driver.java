package fixtures;

/**
 * D073 behavior Driver. Runs exactly one invocation id and prints one JSON
 * object with the invocation id and the observations taken from that real
 * execution. The array contents are read back from the actual array after the
 * call; the exception is the type thrown by the actual call.
 */
public final class Driver {
    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            System.err.println("usage: fixtures.Driver <invocation-id>");
            System.exit(2);
        }
        String invocation = args[0];
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"D073\",\"invocation\":\"").append(invocation).append('"');
        if ("empty-array".equals(invocation)) {
            int[] array = new int[0];
            Case.f(array);
            out.append(",\"returnVoid\":true,\"arrayInt\":[");
            for (int i = 0; i < array.length; i++) {
                if (i > 0) {
                    out.append(',');
                }
                out.append(array[i]);
            }
            out.append(']').append(",\"exception\":null,\"completed\":true");
        } else if ("null-array".equals(invocation)) {
            int[] array = null;
            try {
                Case.f(array);
                out.append(",\"exception\":null,\"completed\":true");
            } catch (Throwable thrown) {
                out.append(",\"exception\":\"").append(thrown.getClass().getName())
                        .append("\",\"completed\":false");
            }
        } else {
            System.err.println("unknown invocation id: " + invocation);
            System.exit(2);
        }
        out.append('}');
        System.out.println(out);
    }
}
