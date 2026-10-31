package fixtures;

/**
 * D064 report scenario. ReportCase is a separate legal owner with the canonical
 * Object constructor and eight distinct static int methods method_0()..method_7(),
 * each returning 0. Every member source is far smaller than the 65536-byte
 * per-member limits of invocation report-total, while the minimum Java length of
 * the eight distinct declarations plus the constructor already exceeds the
 * 128-byte whole-report limit that invocation sets. The methods must stay
 * distinct: no two names or protos may coincide, and none may be deleted.
 */
public final class ReportCase {

    public static int method_0() {
        return 0;
    }

    public static int method_1() {
        return 0;
    }

    public static int method_2() {
        return 0;
    }

    public static int method_3() {
        return 0;
    }

    public static int method_4() {
        return 0;
    }

    public static int method_5() {
        return 0;
    }

    public static int method_6() {
        return 0;
    }

    public static int method_7() {
        return 0;
    }
}
