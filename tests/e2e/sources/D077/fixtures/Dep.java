package fixtures;

/**
 * D077 dependency. ticks starts at the Java default initial value 0 and tick()
 * only performs ticks++. Both are unchanged when the target method is recovered,
 * so the Driver can read the real call effect from the field instead of inferring
 * success from source length or an engine state getter.
 */
public final class Dep {

    public static int ticks;

    public static void tick() {
        ticks = ticks + 1;
    }
}
