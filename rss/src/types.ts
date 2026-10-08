export interface Author {
  name: string;
  email: string;
  url: string;
}

export interface Post {
  id: number;
  name: string;
  permalink: string;
  title: string;
  authors: Author[];
  description: string;
  keywords: string[];
  rss: string[];
  covers: string[];
  datePublished: Date;
  rootPath: string;
  outputPath: string;
  publicModules: string[];
  main: string;
}
