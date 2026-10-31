package fixtures;

import java.util.ArrayList;
import java.util.List;

/** Self-made dependency: records the evaluation order of its own calls. */
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

    public static Receiver receiver() {
        log("R");
        return new Receiver();
    }

    public static int arg(int value) {
        log("A" + value);
        return value;
    }
}
