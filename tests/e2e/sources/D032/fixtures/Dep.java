package fixtures;

import java.util.ArrayList;
import java.util.List;

/** Self-made dependency: a normal producer plus a cleanup that raises. */
public final class Dep {
    public static final List<String> events = new ArrayList<String>();

    private Dep() {
    }

    public static void reset() {
        events.clear();
    }

    public static void log(String label) {
        events.add(label);
    }

    public static int ok() {
        log("OK");
        return 5;
    }

    public static void cleanupThrows() {
        log("CLEANUP");
        throw new ExpectedError();
    }
}
