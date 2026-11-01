public class UnrelatedDriver {
    public static void main(String[] args) {
        System.out.println(fixtures.Invalid.testUnrelated(null));
        System.out.println(fixtures.Invalid.testUnrelated("text"));
        System.out.println(fixtures.Invalid.castUnrelated(null) == null);
        try {
            fixtures.Invalid.castUnrelated("text");
            System.out.println("unexpected return");
        } catch (Throwable e) {
            System.out.println(e.getClass().getName());
        }
    }
}
