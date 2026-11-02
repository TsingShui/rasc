package fixtures;

import java.lang.annotation.ElementType;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;

@Retention(RetentionPolicy.RUNTIME)
@Target(ElementType.TYPE)
public @interface Defaults {
    String value() default "\000\ud800A\udc00\\u000a";
    int[] nums() default {-1, 0, 2147483647};
}
