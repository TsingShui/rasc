package mirror;

/**
 * D065 mirror Driver. Runs exactly one logical member's mirror method and
 * prints one JSON object with the invocation id and the value the call actually
 * returned. The value comes from the real JVM execution of the mirror, never
 * from a copied constant.
 */
public final class Driver {
    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            System.err.println("usage: mirror.Driver <invocation-id>");
            System.exit(2);
        }
        String invocation = args[0];
        int value;
        if ("v1".equals(invocation)) {
            value = logical0.Case.f();
        } else if ("v2".equals(invocation)) {
            value = logical1.Case.f();
        } else {
            System.err.println("unknown invocation id: " + invocation);
            System.exit(2);
            return;
        }
        System.out.println("{\"case\":\"D065\",\"invocation\":\"" + invocation
                + "\",\"result\":{\"kind\":\"int\",\"value\":" + value
                + "},\"exception\":null,\"completed\":true}");
    }
}
