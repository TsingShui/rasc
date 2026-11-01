public class Driver {
    public static void main(String[] args) {
        for (int count : new int[] {0, 1, 3}) {
            for (boolean fail : new boolean[] {false, true}) {
                fixtures.ThrowDep.trace = "";
                System.out.println("loop:" + count + ":" + fail + ":"
                    + fixtures.ShadowState.loop(count, fail) + ":" + fixtures.ThrowDep.trace);
            }
        }
        for (boolean fail : new boolean[] {false, true}) {
            fixtures.ThrowDep.trace = "";
            System.out.println("twoValues:" + fail + ":"
                + fixtures.ShadowState.twoValues(fail) + ":" + fixtures.ThrowDep.trace);
        }
    }
}
