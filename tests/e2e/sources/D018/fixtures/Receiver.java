package fixtures;

/** Self-made receiver whose single method logs its invocation. */
public final class Receiver {
    public int sum(int left, int right) {
        Dep.log("SUM");
        return left + right;
    }
}
