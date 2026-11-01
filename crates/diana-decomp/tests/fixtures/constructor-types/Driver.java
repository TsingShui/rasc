package fixtures;

public final class Driver {
    public static void main(String[] args) {
        Dep.trace = "";
        System.out.println(Case.make("full", Long.MIN_VALUE, -0.0f, 0.5d, true, (byte)-2) != null);
        System.out.println(Dep.trace);
        Dep.trace = "";
        System.out.println(Case.simple("chain") != null);
        System.out.println(Dep.trace);
        Dep.trace = "";
        try { Case.simple(null); } catch (IllegalStateException expected) { System.out.println("failure:" + Dep.trace); }
    }
}
