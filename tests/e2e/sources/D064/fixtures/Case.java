package fixtures;

/**
 * D064 member scenario. f()Ljava/lang/String; returns the four NUL code units
 * the specification binds as invocation v1's stringUnits. The source keeps the
 * literal as four explicit octal escapes so the reference is readable and each
 * unit is visibly NUL, not a Unicode preprocessing newline. The returned value
 * is exactly 4 UTF-16 units; the escaped Java form of that constant is longer
 * than the 16-byte member limit v1 configures.
 */
public final class Case {

    public static String f() {
        return "\0\0\0\0";
    }
}
