package fixtures;

public class Driver {
    public static void main(String[] args) {
        if (Conditions.classNull() != 1 || Conditions.classNonNull() != 1 || Conditions.sameClass() != 1) {
            throw new AssertionError("Class reference comparisons changed");
        }
        System.out.println("class-null, class-non-null, identity: passed");
    }
}
