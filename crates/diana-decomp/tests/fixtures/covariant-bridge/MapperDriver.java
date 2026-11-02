package fixtures;

public class MapperDriver {
    public static void main(String[] args) {
        Mapper value = new StringMapper();
        System.out.println(value.map("x"));
        System.out.println(value.map(null) == null);
        try {
            value.map(new Object());
            System.out.println("missing-cast");
        } catch (ClassCastException expected) {
            System.out.println("cast-failed");
        }
    }
}
