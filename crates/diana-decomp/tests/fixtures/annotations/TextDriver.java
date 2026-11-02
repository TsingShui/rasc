package fixtures;

public final class TextDriver {
    public static void main(String[] args) {
        Mark mark = Text.class.getAnnotation(Mark.class);
        if (mark == null) throw new AssertionError("runtime annotation missing");
        int[] units = new int[mark.value().length()];
        for (int i = 0; i < units.length; i++) units[i] = mark.value().charAt(i);
        System.out.println(java.util.Arrays.toString(units));
        System.out.println(java.util.Arrays.toString(mark.nums()));
    }
}
