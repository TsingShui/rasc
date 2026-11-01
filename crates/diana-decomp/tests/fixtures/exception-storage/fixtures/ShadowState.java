package fixtures;
public class ShadowState {
    public static int loop(int count, boolean fail) {
        int value = 3;
        while (count > 0) {
            value += 2;
            try { ThrowDep.choose(fail); }
            catch (ThrowError ignored) { return value; }
            count--;
        }
        return value;
    }
    public static int twoValues(boolean fail) {
        int first = 7;
        int second = 9;
        try { first = ThrowDep.choose(fail); }
        catch (ThrowError ignored) { return first + second; }
        return first + second;
    }
}
