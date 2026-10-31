package fixtures;

public final class Case {

    public static byte b(boolean c, byte a, byte b) {
        byte v;
        if (c) {
            v = a;
        } else {
            v = b;
        }
        return v;
    }

    public static short s(boolean c, short a, short b) {
        short v;
        if (c) {
            v = a;
        } else {
            v = b;
        }
        return v;
    }

    public static char c(boolean c, char a, char b) {
        char v;
        if (c) {
            v = a;
        } else {
            v = b;
        }
        return v;
    }

    public static int call(boolean c, byte a, byte b) {
        byte v;
        if (c) {
            v = a;
        } else {
            v = b;
        }
        return Dep.echoByte(v);
    }
}
