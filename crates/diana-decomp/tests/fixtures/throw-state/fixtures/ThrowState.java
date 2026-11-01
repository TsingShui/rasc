package fixtures;
public class ThrowState {
    public static int zzHandlers(boolean fail) {
        int value = 7;
        try { value = ThrowDep.choose(fail); }
        catch (IllegalArgumentException ignored) {}
        catch (ThrowError ignored) {}
        catch (Throwable ignored) {}
        return value;
    }
    public static int zzShared(boolean fail) {
        int value = 7;
        try { value = ThrowDep.choose(fail); }
        catch (ThrowError | IllegalArgumentException ignored) {}
        return value;
    }
    public static int zzThrow(boolean fail) {
        int value = 7;
        try {
            try { value = ThrowDep.choose(fail); }
            catch (ThrowError error) { throw error; }
        } catch (Throwable ignored) {}
        return value;
    }
    public static long zzWide(boolean fail) {
        long value = 7;
        try { ThrowDep.choose(fail); value = 11; } catch (ThrowError ignored) {}
        return value;
    }
    public static int overwrite(boolean fail) {
        int value = 7;
        try { value = ThrowDep.choose(fail); } catch (ThrowError ignored) {}
        return value;
    }
    public static int callInside(boolean fail) {
        int value = 7;
        try { value = ThrowDep.choose(fail); } catch (ThrowError ignored) {}
        return value;
    }
    public static int callOutside(boolean fail) {
        int value = 7;
        int result = ThrowDep.choose(fail);
        try { value = result; } catch (ThrowError ignored) {}
        return value;
    }
}
