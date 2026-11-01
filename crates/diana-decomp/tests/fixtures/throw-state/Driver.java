public class Driver {
    public static void main(String[] args) {
        fixtures.ThrowDep.trace = "";
        System.out.println(fixtures.ThrowState.zzHandlers(false) + ":" + fixtures.ThrowDep.trace);
        fixtures.ThrowDep.trace = "";
        System.out.println(fixtures.ThrowState.zzHandlers(true) + ":" + fixtures.ThrowDep.trace);
        fixtures.ThrowDep.trace = "";
        System.out.println(fixtures.ThrowState.zzShared(false) + ":" + fixtures.ThrowDep.trace);
        fixtures.ThrowDep.trace = "";
        System.out.println(fixtures.ThrowState.zzShared(true) + ":" + fixtures.ThrowDep.trace);
        fixtures.ThrowDep.trace = "";
        System.out.println(fixtures.ThrowState.zzThrow(false) + ":" + fixtures.ThrowDep.trace);
        fixtures.ThrowDep.trace = "";
        System.out.println(fixtures.ThrowState.zzThrow(true) + ":" + fixtures.ThrowDep.trace);
        fixtures.ThrowDep.trace = "";
        System.out.println(fixtures.ThrowState.zzWide(false) + ":" + fixtures.ThrowDep.trace);
        fixtures.ThrowDep.trace = "";
        System.out.println(fixtures.ThrowState.zzWide(true) + ":" + fixtures.ThrowDep.trace);
        fixtures.ThrowDep.trace = "";
        System.out.println(fixtures.ThrowState.overwrite(false) + ":" + fixtures.ThrowDep.trace);
        fixtures.ThrowDep.trace = "";
        System.out.println(fixtures.ThrowState.overwrite(true) + ":" + fixtures.ThrowDep.trace);
        fixtures.ThrowDep.trace = "";
        System.out.println(fixtures.ThrowState.callInside(false) + ":" + fixtures.ThrowDep.trace);
        fixtures.ThrowDep.trace = "";
        System.out.println(fixtures.ThrowState.callInside(true) + ":" + fixtures.ThrowDep.trace);
        fixtures.ThrowDep.trace = "";
        System.out.println(fixtures.ThrowState.callOutside(false) + ":" + fixtures.ThrowDep.trace);
        fixtures.ThrowDep.trace = "";
        try { fixtures.ThrowState.callOutside(true); }
        catch (fixtures.ThrowError e) { System.out.println(e.getClass().getName() + ":" + fixtures.ThrowDep.trace); }
    }
}
