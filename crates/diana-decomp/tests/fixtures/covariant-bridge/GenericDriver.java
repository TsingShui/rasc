package fixtures;

public class GenericDriver {
    public static void main(String[] args) {
        Object value = new Object();
        System.out.println(Caller.call(value) == value);
        System.out.println(Caller.call(null) == null);
        System.out.println(Caller.call("x"));
    }
}
