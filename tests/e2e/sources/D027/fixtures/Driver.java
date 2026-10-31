package fixtures;

/**
 * D027 behavior Driver. Runs the case fixture for exactly one invocation id and
 * prints one JSON object with the invocation id and the observations taken from
 * that real execution. The same Driver runs the reference Java mirror and, after
 * recovery, the recovered source, so the observations are not copied constants.
 */
public final class Driver {
    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            System.err.println("usage: fixtures.Driver <invocation-id>");
            System.exit(2);
        }
        String invocation = args[0];
        StringBuilder out = new StringBuilder();
        out.append("{\"case\":\"D027\",\"invocation\":\"").append(invocation).append('"');
        if ("v1".equals(invocation)) {
            int[] a = Case.a();
            out.append(",\"result\":{\"kind\":\"int[]\"},\"arrayInt\":[");
            for (int i = 0; i < a.length; i++) {
                if (i > 0) {
                    out.append(',');
                }
                out.append(a[i]);
            }
            out.append(']');
        } else if ("v2".equals(invocation)) {
            float[] b = Case.b();
            out.append(",\"result\":{\"kind\":\"float[]\"},\"arrayFloatBits\":[");
            for (int i = 0; i < b.length; i++) {
                if (i > 0) {
                    out.append(',');
                }
                out.append('"').append(hex32(Float.floatToRawIntBits(b[i]))).append('"');
            }
            out.append(']');
        } else {
            System.err.println("unknown invocation id: " + invocation);
            System.exit(2);
        }
        out.append(",\"exception\":null,\"completed\":true}");
        System.out.println(out);
    }

    private static String hex32(int bits) {
        String hex = Integer.toHexString(bits);
        StringBuilder padded = new StringBuilder();
        for (int i = hex.length(); i < 8; i++) {
            padded.append('0');
        }
        return padded.append(hex).toString();
    }
}
