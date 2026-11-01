package fixtures;
public final class Protected {
    public static int rethrow(boolean fail, boolean cleanupFail) throws Checked {
        try { return Dep.choose(fail); }
        catch (Throwable error) { Dep.cleanup(cleanupFail); throw error; }
    }
    public static int catchAll(boolean fail) {
        try { return Dep.choose(fail); }
        catch (Throwable error) { return 13; }
    }
    public static int typed(boolean fail) {
        try { return Dep.choose(fail); }
        catch (Checked error) { Dep.cleanup(false); return 17; }
    }
    public static int outside(boolean cleanupFail) {
        int value;
        try { value = Dep.choose(false); }
        catch (Checked error) { value = -1; }
        Dep.cleanup(cleanupFail);
        return value;
    }
}
