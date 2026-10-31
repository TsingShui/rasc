package fixtures;

import java.util.ArrayList;
import java.util.List;

/** Self-made dependency: logs INIT and then fails the static initializer. */
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

    public static int badInit() {
        log("INIT");
        throw new InitError();
    }
}
