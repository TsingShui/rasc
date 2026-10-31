package fixtures;

/**
 * D086 independent reference Java. f()V returns directly, so the only
 * observable behavior of the original method is a normal void return; the
 * 131072 dead nops in the DEX code range carry no behavior and must not
 * become Java statements. The recovered method is compiled and executed by
 * the Driver against this same shape.
 */
public final class Case {

    public static void f() {
    }
}
