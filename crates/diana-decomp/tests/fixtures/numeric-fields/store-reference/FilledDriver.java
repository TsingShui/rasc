public class FilledDriver {
    public static void main(String[] args) {
        Object first = new Object(); Object second = new Object();
        Object[] a = fixtures.Invalid.filledObjects(first, second);
        System.out.println(a.length + ":" + (a[0] == first) + ":" + (a[1] == second));
        Object[] n = fixtures.Invalid.filledObjects(null, first);
        System.out.println((n[0] == null) + ":" + (n[1] == first));
        System.out.println(java.util.Arrays.toString(fixtures.Invalid.filledInts(Integer.MIN_VALUE, Integer.MAX_VALUE)));
        System.out.println(java.util.Arrays.toString(fixtures.Invalid.filledRange(1, -2, 3, -4, 5, -6)));
        System.out.println(java.util.Arrays.toString(fixtures.Invalid.filledReuse(17, 29)));
        Object[] z = fixtures.Invalid.filledZero(); Object[] another = fixtures.Invalid.filledZero();
        System.out.println(z.length + ":" + (z != another) + ":" + z.getClass().getName());
        fixtures.Invalid.filledUnused(Integer.MIN_VALUE, Integer.MAX_VALUE);
        System.out.println("unused returned");
    }
}
