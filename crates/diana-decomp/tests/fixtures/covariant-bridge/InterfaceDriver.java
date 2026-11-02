package fixtures;

public class InterfaceDriver {
    public static void main(String[] args) {
        Marker value = new RawImpl();
        System.out.println(value.value());
    }
}
