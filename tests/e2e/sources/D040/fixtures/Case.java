package fixtures;

/** Target: the declared synchronized methods lock this and the class object. */
public class Case {
    public synchronized boolean inst() {
        return Thread.holdsLock(this);
    }

    public static synchronized boolean stat() {
        return Thread.holdsLock(Case.class);
    }
}
