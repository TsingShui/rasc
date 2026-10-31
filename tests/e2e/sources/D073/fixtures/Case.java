package fixtures;

/**
 * Independent reference Java mirror for D073 (spec inputRole "original" is the
 * smali fixture Case.smali). The mirror is the closed semantic counterpart the
 * specification names: one length read of the argument array and no write.
 * It is not an execution of the original DEX and it must not be generalized
 * into an exception or lock interpreter.
 */
public class Case {
    public Case() {
    }

    public static void f(int[] a) {
        int unused = a.length;
    }
}
