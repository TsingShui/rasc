package fixtures;

public final class Case {

    public static boolean and() {
        return Dep.a() && Dep.b();
    }

    public static boolean or() {
        return Dep.a() || Dep.b();
    }
}
