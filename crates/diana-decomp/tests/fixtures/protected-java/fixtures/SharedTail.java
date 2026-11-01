package fixtures;
public class SharedTail {
    public static int f(boolean fail) {
        try { return Dep.choose(fail); }
        catch (Checked error) {}
        catch (Throwable error) {}
        Dep.cleanup(false);
        return 5;
    }
}
