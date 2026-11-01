package fixtures;

public class AbruptDriver {
    public static void main(String[] args) {
        try {
            new Abrupt();
            throw new AssertionError("initialization completed");
        } catch (ExceptionInInitializerError error) {
            if (!(error.getCause() instanceof NullPointerException)) {
                throw new AssertionError(error);
            }
        }
        try {
            new Abrupt();
            throw new AssertionError("erroneous class initialized again");
        } catch (NoClassDefFoundError expected) {
            System.out.println("initialization-failed; erroneous-class-retained");
        }
    }
}
