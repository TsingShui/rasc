package fixtures;

public class Driver {
    public static void main(String[] args) {
        System.out.println(new Case().x + ":" + new Case(11).x);
        System.out.println(Prepared.N + ":" + Prepared.S + ":" + Prepared.Z);
        System.out.println(Prepared.b + ":" + (int) Prepared.c + ":" + Prepared.d + ":"
            + Float.floatToRawIntBits(Prepared.f) + ":" + Prepared.i + ":" + Prepared.j);
        System.out.println(Prepared.s.length() + ":" + (int) Prepared.s.charAt(1) + ":"
            + (int) Prepared.s.charAt(2));
    }
}
