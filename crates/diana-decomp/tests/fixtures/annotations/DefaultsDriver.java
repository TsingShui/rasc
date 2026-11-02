package fixtures;

import java.util.Arrays;

public final class DefaultsDriver {
    public static void main(String[] args) throws Exception {
        Defaults annotation = Defaulted.class.getAnnotation(Defaults.class);
        System.out.println(Arrays.toString(annotation.value().chars().toArray()));
        System.out.println(Arrays.toString(annotation.nums()));
        int[] changed = annotation.nums();
        changed[0] = 99;
        System.out.println(annotation.nums()[0]);
        String value = (String) Defaults.class.getMethod("value").getDefaultValue();
        System.out.println(Arrays.toString(value.chars().toArray()));
        System.out.println(Arrays.toString((int[]) Defaults.class.getMethod("nums").getDefaultValue()));
        Defaults explicit = Override.class.getAnnotation(Defaults.class);
        System.out.println(explicit.value() + ":" + explicit.nums().length);
        System.out.println(Defaults.class.getAnnotation(java.lang.annotation.Retention.class).value());
        System.out.println(Arrays.toString(Defaults.class.getAnnotation(java.lang.annotation.Target.class).value()));
    }
}
