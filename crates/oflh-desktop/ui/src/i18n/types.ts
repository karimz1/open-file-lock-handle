export type TranslationSchema<T> = {
  [Key in keyof T]: T[Key] extends string ? string : TranslationSchema<T[Key]>;
};

export type MessagePaths<T> = {
  [Key in keyof T & string]: T[Key] extends string
    ? Key
    : `${Key}.${MessagePaths<T[Key]>}`;
}[keyof T & string];
