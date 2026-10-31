package fixtures;

/**
 * Independent reference Java mirror for D027 (spec inputRole "original" is the
 * smali fixture Case.smali). The mirror states the element values and bit
 * patterns the original new-array + fill-array-data pair must transfer; it is
 * not an execution of the original DEX and it does not select the DEX payload
 * form.
 */
public class Case {
    public Case() {
    }

    public static int[] a() {
        return new int[] {-1, 0, 2147483647};
    }

    public static float[] b() {
        return new float[] {-0.0f, 1.0f};
    }
}
