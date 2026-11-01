public class StoreDriver {
    public static void main(String[] args) {
        byte[] b = new byte[1];
        System.out.println(fixtures.Invalid.storeByte(b, 255) + ":" + b[0]);
        System.out.println(fixtures.Invalid.storeByte(b, 256) + ":" + b[0]);
        System.out.println(fixtures.Invalid.storeByte(b, -129) + ":" + b[0]);
        boolean[] z = new boolean[1];
        System.out.println(fixtures.Invalid.booleanProof(z, -1) + ":" + z[0]);
        System.out.println(fixtures.Invalid.booleanProof(z, 2) + ":" + z[0]);
        System.out.println(fixtures.Invalid.booleanProof(z, 3) + ":" + z[0]);
    }
}
