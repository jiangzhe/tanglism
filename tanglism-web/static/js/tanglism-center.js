// 定义中枢相关函数
// 依赖jquery, jquery-ui, d3, tanglism-common, tanglism-kline
export const center = {
    data,
    table,
    clear_table,
    draw,
    outdate
};

import { kline } from './tanglism-kline.js';

const _data = [];
var _outdate = true;

function data(input) {
    if (input) {
      while (_data.length > 0) { _data.pop(); }
      for (var i = 0; i < input.length; i++) {
        _data.push(input[i]);
      }
      _outdate = false;
      return;
    }
    return _data;
}

function table() {
    // 创建表格
    if (!d3.select("#cr_table").empty()) {
      d3.select("#cr_table").remove();
    }
    var table = d3.select("#cr_container").append("table")
      .attr("id", "cr_table")
      .style("border-collapse", "collapse")
      .style("border", "2px black solid");
    // 表头
    table.append("thead")
      .append("tr")
      .selectAll("th")
      .data(["起始时刻", "起始价格", "终止时刻", "终止价格", "区间最低", "区间最高", "最低", "最高", "方向"])
      .enter()
      .append("th")
      .text(function(d) {return d;})
      .style("border", "1px black solid")
      .style("padding", "5px")
      .style("background-color", "lightgray")
      .style("font-weight", "bold");
    // 内容
    table.append("tbody")
      .selectAll("tr")
      .data(_data)
      .enter()
      .append("tr")
      .selectAll("td")
      .data(function(d) {
        return [
          d.start.ts, 
          d.start.value, 
          d.end.ts, 
          d.end.value,
          d.shared_low.value,
          d.shared_high.value,
          d.low.value,
          d.high.value,
          d.upward ? "上升" : "下降"
        ];
      })
      .enter()
      .append("td")
      .style("border", "1px black solid")
      .style("padding", "5px")
      .style("font-size", "12px")
      .text(function(d) {return d;})
      .on("mouseover", function(){
        d3.select(this).style("background-color", "powderblue");
      })
      .on("mouseout", function(){
        d3.select(this).style("background-color", "white");
      });
}

function clear_table() {
    // 删除表格
    d3.select("#cr_table").remove();
}

function draw(config) {
    // 是否在图中显示线段
    var center_draw_check = $("#center_draw").is(":checked");
    if (!center_draw_check) {
      return;
    }
    if (_outdate) {
      console.log("center outdate");
      return;
    }
    var conf = config || kline.conf();
    // 无K线图，直接退出
    // if (d3.select("#k_lines").empty()) {
    //   return;
    // }
    // 无K线数据或中枢数据，直接退出
    if (kline.data().length == 0 || _data.length == 0) {
      return;
    }
    // 查询中枢对应K线位置，并添加序列号
    var ki = 0;
    var ci = 0;
    var start_match = false;
    var kdata = kline.data();
    while (ci < _data.length && ki < kdata.length) {
      var cr = _data[ci];
      var k = kdata[ki];
      if (!start_match && cr.start.ts === k.ts) {
        // 起点序列号
        cr.start_id = ki;
        // 将start_match置为true
        start_match = true;
      } else if (cr.end.ts === k.ts) {
        // 终点序列号
        cr.end_id = ki;
        // 仅递增线段，下一中枢起点应与前一线段终点一致，需复用ki
        ci++;
        start_match = false;
      } else {
        // 未匹配到，K线号递增
        ki++;
      }
    }

    // 过滤出所有匹配上的线段
    var centers = [];
    for (var i = 0; i < _data.length; i++) {
      var item = _data[i];
      if (item.hasOwnProperty("start_id") && item.hasOwnProperty("end_id")) {
        centers.push(item);
      }
    }
    var svg = d3.select("#k_lines");
    svg.selectAll("rect.center")
        .data(centers)
        .enter()
        .append("rect")
        .attr("class", "center")
        .attr("x", function(d, i) {
            return d.start_id * conf.bar_width;
        })
        .attr("y", function(d, i) {
            return conf.h - conf.yscale(d.shared_high.value);
        })
        .attr("width", function(d) {
            return conf.bar_width * (d.end_id - d.start_id);
        })
        .attr("height", function(d) {
            return Math.max(1, Math.abs(conf.yscale(d.shared_high.value) - conf.yscale(d.shared_low.value)));
        })
        .attr("fill", "gold")
        .attr("opacity", 0.5);
};

function outdate() {
  _outdate = true;
}