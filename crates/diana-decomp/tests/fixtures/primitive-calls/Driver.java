package fixtures;

public final class Driver {
    public static void main(String[] args) {
        Dep receiver = new Dep();
        Dep.trace = 0;
        System.out.println(Case.longCall(Long.MIN_VALUE));
        System.out.println(Float.floatToRawIntBits(Case.floatCall(Float.intBitsToFloat(0x7fc00001))));
        System.out.println(Double.doubleToRawLongBits(Case.doubleCall(-0.0d)));
        System.out.println("trace:" + Dep.trace);
        Dep.trace = 0;
        System.out.println(Float.floatToRawIntBits(Case.literalFloat()));
        System.out.println(Double.doubleToRawLongBits(Case.literalDouble()));
        System.out.println(Case.mixed(receiver, 7L, 0.5f, 2.25d, (byte)-3));
        System.out.println(Case.text(receiver, "word"));
        Case.unused(1.5f);
        Case.receiverVoid(receiver, 0.0d);
        System.out.println("trace:" + Dep.trace);
        try { Case.mixed(null, 7L, 0.5f, 2.25d, (byte)-3); } catch (NullPointerException caught) { System.out.println("receiver:NPE"); }
    }
}
