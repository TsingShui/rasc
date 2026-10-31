package fixtures;

/**
 * D046 independent reference Java. It declares only the three non-final static
 * fields of the DEX in the same declaration order (b:Z, i:I, s:Ljava/lang/String;)
 * and gives none of them an initializer, so the JVM default state is the whole
 * observation. The DEX built by build_dex.py encodes the same three defaults
 * explicitly in static_values; the mirror does not claim that javac emits that
 * encoded array.
 */
public class Case {
    public static boolean b;
    public static int i;
    public static String s;

    public Case() {
    }
}
