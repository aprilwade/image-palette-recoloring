import numpy as np
from scipy.spatial import ConvexHull, Delaunay
from PIL import Image

im = Image.open("benches/test_image.png")

w, h = im.size

points = []
for x in range(w):
    for y in range(h):
        pixel = im.getpixel((x, y))
        points.append((pixel[0], pixel[1], pixel[2], x, y))


points.sort()
# for point in points:
#     print(list(point))
points = np.array(points)
# for point in points:
#     print(list(point))
hull = ConvexHull(points)
# for vertex in hull.points[hull.vertices]:
#     print(list(vertex))
tri = Delaunay(hull.points[hull.vertices])
# print(len(hull.vertices))
# print(len(tri.simplices))

# print(tri.points[tri.simplices]))
print(tri.points[tri.simplices[0]])

# print(len(tri.simplices))
# print(tri.nsimplex)
# for simplex in tri.simplices:
#     print([list(tri.points[i]) for i in simplex])


'''
for i, point in enumerate(points):
    # if i == 10:
    if i == 10000:
        break
    print([float(p) for p in point])
    n = tri.find_simplex(point, bruteforce=True)
    simplex = tri.simplices[n]
    print([list(m) for m in tri.points[simplex]])
'''

'''
point = points[2]
n = tri.find_simplex(point, bruteforce=True)
simplex = tri.simplices[n]
print([list(m) for m in tri.points[simplex]])
'''

# print(tri.points[tri.simplices[0]])
# for transform in tri.transforms:
#     printkj


# print(tri.neighbors[0])
