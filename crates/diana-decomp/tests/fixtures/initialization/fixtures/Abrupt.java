package fixtures;

public class Abrupt {
    public Abrupt() {}
    static {
        if (true) {
            throw (NullPointerException) null;
        }
    }
}
