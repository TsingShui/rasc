package fixtures;

import java.util.ArrayList;
import java.util.List;

/** Self-made dependency: logs B, bumps the counter, then raises. */
public final class Dep {
    public static final List<String> events = new ArrayList<String>();
    public static int counter = 0;

    private Dep() {
    }

    public static void reset() {
        events.clear();
        counter = 0;
    }

    public static void log(String label) {
        events.add(label);
    }

    public static void bumpAndThrow() {
        log("B");
        counter++;
        throw new ExpectedError();
    }
}
