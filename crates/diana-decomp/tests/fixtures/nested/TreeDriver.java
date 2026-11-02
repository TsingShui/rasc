public class TreeDriver {
    public static void main(String[] args) {
        System.out.println(new fixtures.Tree().read());
        System.out.println(new fixtures.Tree.A.Same().f());
        System.out.println(new fixtures.Tree.B.Same().f());
    }
}
