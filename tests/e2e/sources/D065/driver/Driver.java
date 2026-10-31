package driver;

import fixtures.Case;

/** Executes the recovered original owner, never either reference mirror. */
public final class Driver {
    public static void main(String[] args) {
        if (args.length != 1 || !(args[0].equals("v1") || args[0].equals("v2"))) {
            throw new IllegalArgumentException("unknown invocation");
        }
        int value = Case.f();
        System.out.println("{\"case\":\"D065\",\"invocation\":\"" + args[0]
            + "\",\"result\":{\"kind\":\"int\",\"value\":" + value
            + "},\"exception\":null,\"completed\":true}");
    }
}
