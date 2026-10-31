package fixtures;

/** Target reader: each method logs READ first and then reads the Case field. */
public final class Reader {
    public static int readInt() {
        Dep.log("READ");
        return Case.N;
    }

    public static String readString() {
        Dep.log("READ");
        return Case.S;
    }
}
