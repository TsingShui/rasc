package fixtures;

import java.util.ArrayList;
import java.util.List;

/** Self-made dependency: the index computation is the observable effect prefix. */
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

    public static int index() {
        log("IDX");
        return 2;
    }
}
