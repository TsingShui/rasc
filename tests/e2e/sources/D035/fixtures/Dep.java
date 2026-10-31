package fixtures;

import java.util.ArrayList;
import java.util.List;

/** Self-made dependency: two raising producers plus the outer-handler observer. */
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

    public static void failA() {
        log("A");
        throw new ErrorA();
    }

    public static void failB() {
        log("B");
        throw new ErrorB();
    }
}
