package fixtures;

import java.util.ArrayList;
import java.util.List;

/** Self-made dependency: logs its own calls and raises the expected error. */
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

    public static int alwaysThrows() {
        log("CALL");
        throw new ExpectedError();
    }

    public static void record(int value) {
        log("OLD:" + value);
    }
}
