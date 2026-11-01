public class ReferenceDriver {
    static void observe(boolean nil, boolean fail, int index) {
        Object old = new Object(); Object next = new Object();
        fixtures.Box peer = new fixtures.Box(); fixtures.Box replacement = new fixtures.Box();
        fixtures.Box b = nil ? null : new fixtures.Box();
        float[] values = new float[]{-0.0f}; float[] other = new float[]{1.0f};
        if (b != null) { b.object = old; b.text = "old"; b.peer = peer; b.values = values; }
        fixtures.Box.staticObject = old; fixtures.Box.staticPeer = peer;
        fixtures.Dep.fail = fail; fixtures.Dep.calls = 0;
        try { System.out.print("object=" + (fixtures.Case.exchangeObject(b, next) == old) + ";"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try { System.out.print("string=" + (fixtures.Case.exchangeString(b, "next") == "old") + ";"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try { System.out.print("peer=" + (fixtures.Case.exchangePeer(b, replacement) == peer) + ";"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try { System.out.print("array=" + (fixtures.Case.exchangeValues(b, other) == values) + ";"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try { System.out.print("element=" + Integer.toHexString(Float.floatToRawIntBits(fixtures.Case.fieldArray(b, index, 2.0f))) + ";"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try { System.out.print("static=" + (fixtures.Case.staticReference(next) == old) + ";"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        try { fixtures.Case.unusedReferences(b); System.out.print("unused;"); }
        catch (Throwable e) { System.out.print(e.getClass().getName() + ";"); }
        System.out.println("calls=" + fixtures.Dep.calls + ";stored=" + (b == null ? "null" : (b.object == next) + ":" + (b.text == "next") + ":" + (b.peer == replacement) + ":" + (b.values == other)) + ";static=" + (fixtures.Box.staticObject == next) + ";bits=" + Integer.toHexString(Float.floatToRawIntBits(other[0])));
        if (b != null) {
            System.out.println("widen=" + (fixtures.Case.widenPeer(b, replacement) == replacement));
            System.out.println("null=" + (fixtures.Case.nullReferences(b) == null) + ":" + (b.object == null) + ":" + (b.text == null) + ":" + (b.values == null) + ":" + (fixtures.Box.staticPeer == null));
        }
    }
    public static void main(String[] args) {
        observe(false, false, 0);
        observe(false, true, 0);
        observe(true, false, 0);
        observe(false, false, -1);
    }
}
