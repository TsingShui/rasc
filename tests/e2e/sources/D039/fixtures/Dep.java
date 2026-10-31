package fixtures;

import java.util.ArrayList;
import java.util.List;

/** Self-made dependency: nextLock logs LOCK and counts its own invocations. */
public final class Dep {
    public static final List<String> events = new ArrayList<String>();
    public static int lockCalls = 0;

    private static final Object LOCK = new Object();

    private Dep() {
    }

    public static void reset() {
        events.clear();
        lockCalls = 0;
    }

    public static void log(String label) {
        events.add(label);
    }

    public static Object nextLock() {
        lockCalls++;
        log("LOCK");
        return LOCK;
    }
}
