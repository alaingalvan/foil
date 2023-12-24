export type Post = {
  permalink: string;
  title: string;
  description: string;
  keywords: string[];
  authors: { name: string; email: string; url: string }[];

  main: string;
  data: any;
  rootPermalink: string;
  datePublished: Date;
  dateModified: Date;

  // Metadata for post resolution
  meta: {
    rootPath: string;
    dateModified: Date;
    files: { path: string; modified: Date }[];
  };
};

export type Loader = {
  test: {
    [key: string]: RegExp;
  };
  transform: (post: Post) => Promise<Post>;
};
